(declare-fun in () Int)
(declare-fun out1 () Real)
(declare-fun out2 () Real)
(declare-fun l1 () Int)
(declare-fun l2 () Int)
(declare-fun lin () Int)

(define-fun costf ((a Real) (b Real) (c Real)) Bool
    (= 
        c 
        (abs (- a b))
    )
)

(define-fun inv ((a Int) (b Real) (c Real)) Bool
    (=
        a
        (+ b c)
    )
)

(define-fun is_minimal ((c_min Real)) Bool
    (forall ((z Real) (o1 Real) (o2 Real))
        (=>
            (and
                (inv in o1 o2)
                (costf o1 o2 z)
            )
            (>= z c_min)
        )
    )
)

(define-fun min2 ((a Real) (b Real)) Real
    (ite (>= a b) b a)
)

(assert (inv in out1 out2))
(assert (<= 0 in 15))
(assert (<= 0 out1 15))
(assert (<= 0 out2 15))


(define-fun to_prove ((cost Real)) Bool 
    (=> 
        (or 
            (= l1 l2) 
            (<= lin (* 2 (min2 l1 l2)))
        ) 
        (= cost 0)
    )
)
; l1, l2:   output capacity
; lin:      input capacity
; cost:     cost of the splitter
; To prove:
; l1 = l2 || lin <= 2 * min(l1, l2)
; => 
; cost = 0
(assert
    (= false (exists ((c Real))
        (and 
            (=>
                (costf out1 out2 c)
                (is_minimal c)
            )
            (to_prove c)
        )
    ))
)

(check-sat)
(get-model)